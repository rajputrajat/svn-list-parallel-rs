use async_recursion::async_recursion;
use async_std::task;
use log::info;
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
            data: self,
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
        loop {
            if let Some(outer) = self.data.0.iter().nth(self.outer_index) {
                if let Some(inner) = outer.iter().nth(self.inner_index) {
                    self.inner_index += 1;
                    return Some(inner);
                } else {
                    self.outer_index += 1;
                }
            } else {
                return None;
            }
        }
    }
}

pub trait ListParallel {
    fn list_parallel(&self, path: &str) -> Result<Arc<Mutex<SvnListParallel>>, SvnError>;
}

impl ListParallel for SvnCmd {
    fn list_parallel(&self, path: &str) -> Result<Arc<Mutex<SvnListParallel>>, SvnError> {
        let all_svn_list = Arc::new(Mutex::new(SvnListParallel(LinkedList::new())));
        let mut result: Result<(), SvnError> = Ok(());
        task::block_on(async {
            result = run_parallely(self.clone(), path.to_owned(), all_svn_list.clone()).await;
        });
        result.map(|_| all_svn_list)
    }
}

#[async_recursion]
async fn run_parallely(cmd: SvnCmd, path: String, big_list: AtomicList) -> Result<(), SvnError> {
    let svn_list = cmd.list(&path, false).await?;
    info!("{:#?}", svn_list);
    let mut tasks = Vec::new();
    for item in svn_list.iter() {
        if item.kind == PathType::Dir {
            let path = path.clone();
            let new_path = format!("{}/{}", &path, item.name);
            let cmd = cmd.clone();
            let big_list = big_list.clone();
            tasks.push(task::spawn(async move {
                run_parallely(cmd, new_path, big_list).await
            }));
        }
    }
    for task in tasks {
        task.await?;
    }
    big_list.lock().unwrap().0.push_back(svn_list);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use svn_cmd::{EntryCommit, ListsList};

    #[test]
    fn test_iter() {
        let mut lists_vec = VecDeque::new();
        let e1 = ListEntry {
            kind: PathType::Dir,
            name: "first".to_owned(),
            size: None,
            commit: EntryCommit {
                revision: 100,
                author: "Lemon".to_owned(),
                date: "20/10/2001".to_owned(),
            },
        };
        let e2 = ListEntry {
            kind: PathType::File,
            name: "second".to_owned(),
            size: None,
            commit: EntryCommit {
                revision: 101,
                author: "Lemon".to_owned(),
                date: "20/10/2001".to_owned(),
            },
        };
        let e3 = ListEntry {
            kind: PathType::Dir,
            name: "third".to_owned(),
            size: None,
            commit: EntryCommit {
                revision: 102,
                author: "Lemon".to_owned(),
                date: "20/10/2001".to_owned(),
            },
        };
        lists_vec.push_back(e1.clone());
        lists_vec.push_back(e2.clone());
        lists_vec.push_back(e3.clone());
        let mut list = LinkedList::new();
        list.push_back(SvnList {
            list: ListsList { entry: lists_vec },
        });
        let value: AtomicList = Arc::new(Mutex::new(SvnListParallel(list)));
        let list_struct = value.lock().unwrap();
        let mut iter = list_struct.iter();
        assert_eq!(iter.next(), Some(&e1));
        assert_eq!(iter.next(), Some(&e2));
        assert_eq!(iter.next(), Some(&e3));
        assert_eq!(iter.next(), None);
    }
}
