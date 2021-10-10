use async_recursion::async_recursion;
use async_std::task;
use std::{
    collections::LinkedList,
    sync::{Arc, Mutex},
};
use svn_cmd::{ListEntry, PathType, SvnCmd, SvnError, SvnList};

type AtomicList = Arc<Mutex<SvnListParallel>>;

#[derive(Debug)]
pub struct SvnListParallel(LinkedList<SvnList>);

impl SvnListParallel {
    pub fn iter(&self) -> ListEntryIterator {
        ListEntryIterator {
            data: &self,
            outer_index: 0,
            inner_index: 0,
        }
    }
}

pub struct ListEntryIterator<'a> {
    data: &'a SvnListParallel,
    outer_index: usize,
    inner_index: usize,
}

impl<'a> Iterator for ListEntryIterator<'a> {
    type Item = &'a ListEntry;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(outer) = self.data.0.iter().nth(self.outer_index) {
            if let Some(inner) = outer.iter().nth(self.inner_index) {
                Some(inner)
            } else {
                None
            }
        } else {
            None
        }
    }
}

pub trait ListParallel {
    fn list_parallel(&self, path: &str) -> Arc<Mutex<SvnListParallel>>;
}

impl ListParallel for SvnCmd {
    fn list_parallel(&self, path: &str) -> Arc<Mutex<SvnListParallel>> {
        let all_svn_list = Arc::new(Mutex::new(SvnListParallel(LinkedList::new())));
        {
            task::block_on(async {
                run_parallely(self.clone(), path.to_owned(), all_svn_list.clone())
                    .await
                    .unwrap();
            });
        }
        all_svn_list
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
