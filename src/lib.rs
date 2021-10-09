use async_recursion::async_recursion;
use async_std::task;
use std::{
    collections::LinkedList,
    sync::{Arc, Mutex},
};
use svn_cmd::{ListEntry, PathType, SvnCmd, SvnError, SvnList};

type AtomicList = Arc<Mutex<SvnListParallel>>;

pub struct SvnListParallel(LinkedList<SvnList>);

pub trait ListParallel {
    fn list_parallel(&self, path: &str) -> Result<SvnListParallel, SvnError>;
}

impl ListParallel for SvnCmd {
    fn list_parallel(&self, path: &str) -> Result<SvnListParallel, SvnError> {
        let all_svn_list = Arc::new(Mutex::new(SvnListParallel(LinkedList::new())));
        task::block_on(async {
            run_parallely(self.clone(), path.to_owned(), all_svn_list.clone())
                .await
                .unwrap();
        });
        all_svn_list
            .into_inner()
            .map_err(|e| SvnError::Other(e.to_string()))
    }
}

#[async_recursion]
async fn run_parallely(cmd: SvnCmd, path: String, big_list: AtomicList) -> Result<(), SvnError> {
    let svn_list = cmd.list(&path, false).await?;
    let mut tasks = Vec::new();
    for item in svn_list.iter() {
        if item.kind == PathType::Dir {
            let path = path.clone();
            let new_path = format!("{}/{}", &path, item.name);
            let cmd = cmd.clone();
            let big_list = big_list.clone();
            tasks.push(task::spawn(async move {
                match run_parallely(cmd, new_path, big_list).await {
                    Ok(o) => o,
                    Err(_) => {}
                }
            }));
        }
    }
    for task in tasks {
        task.await;
    }
    big_list.lock().unwrap().0.push_back(svn_list);
    Ok(())
}
