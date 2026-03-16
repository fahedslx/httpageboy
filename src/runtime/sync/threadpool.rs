use std::fmt::{Display, Formatter, Result};
use std::sync::mpsc::{Receiver, SendError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread::{JoinHandle, spawn};

type Job = Box<dyn FnOnce() + Send + 'static>;

enum Message {
  NewJob(Job),
  Terminate,
}

#[allow(dead_code)]
struct Worker {
  id: usize,
  thread: Option<JoinHandle<()>>,
}

impl Worker {
  fn new(id: usize, receiver: Arc<Mutex<Receiver<Message>>>) -> Worker {
    let thread = spawn(move || {
      loop {
        let message = match receiver.lock() {
          Ok(lock) => match lock.recv() {
            Ok(message) => message,
            Err(_) => break,
          },
          Err(_) => break,
        };

        match message {
          Message::NewJob(job) => {
            // println!("Worker {} got a job; executing.", id);
            job();
          }
          Message::Terminate => {
            // println!("Worker {} was told to terminate.", id);
            break;
          }
        }
      }
    });
    Worker {
      id,
      thread: Some(thread),
    }
  }
}

pub struct ThreadPool {
  workers: Vec<Worker>,
  sender: Sender<Message>,
}

impl Display for ThreadPool {
  fn fmt(&self, f: &mut Formatter) -> Result {
    write!(f, "ThreadPool {}", self.workers.len())
  }
}

impl ThreadPool {
  pub fn new(size: usize) -> ThreadPool {
    assert!(size > 0);

    let (sender, receiver) = channel();
    let receiver: Arc<Mutex<Receiver<Message>>> = Arc::new(Mutex::new(receiver));
    let mut workers: Vec<Worker> = Vec::with_capacity(size);

    for id in 0..size {
      workers.push(Worker::new(id, Arc::clone(&receiver)));
    }

    ThreadPool { workers, sender }
  }

  pub fn run<F>(&self, _f: F)
  where
    F: FnOnce() + Send + 'static,
  {
    let job = Box::new(_f);
    if let Err(err) = self.sender.send(Message::NewJob(job)) {
      match err {
        // SendError(_) => println!("Error sending job to thread pool."),
        SendError(_) => (),
      }
    }
  }

  pub fn stop(&mut self) {
    for _ in &self.workers {
      let _ = self.sender.send(Message::Terminate);
    }

    for worker in &mut self.workers {
      if let Some(thread) = worker.thread.take() {
        if let Err(_e) = thread.join() {
          // println!("Error joining thread: {:?}", _e);
        }
      }
    }
  }
}

impl Drop for ThreadPool {
  fn drop(&mut self) {
    self.stop();
  }
}
