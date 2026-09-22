//the table of commands running in the background
//a job is shown as Done exactly once, then forgotten

use std::io::Write;
use std::process::Child;

pub struct Job {
    pub number: usize,
    pub child: Child,
    pub command: String,                                                         //the command line as typed, without the "&"
}

//register a job that was just started and announce it: "[1] 12345"
pub fn add(jobs: &mut Vec<Job>, child: Child, command: String) {
    let number = next_number(jobs);
    println!("[{number}] {}", child.id());
    jobs.push(Job { number, child, command });                                   //the new number is the highest, so the table stays sorted
}

//numbers are recycled: one more than the highest number still in use, 1 when the table is empty
fn next_number(jobs: &[Job]) -> usize {
    let mut highest = 0;
    for job in jobs {
        highest = highest.max(job.number);
    }
    highest + 1
}

//check every job, print its line, and forget the ones that finished
//the jobs builtin shows every job, the check before each prompt only the finished ones
pub fn report(jobs: &mut Vec<Job>, out: &mut dyn Write, show_running: bool) {
    let mut finished = Vec::new();
    for job in jobs.iter_mut() {
        //try_wait() does not block: Some means the process has exited, and it is reaped
        finished.push(matches!(job.child.try_wait(), Ok(Some(_))));
    }

    //markers are computed on the table as it is now, before anything is removed
    let count = jobs.len();
    for (i, job) in jobs.iter().enumerate() {
        let marker = marker(i, count);

        if finished[i] {
            writeln!(out, "[{}]{}  {:<24}{}", job.number, marker, "Done", job.command).ok();
        } else if show_running {
            writeln!(out, "[{}]{}  {:<24}{} &", job.number, marker, "Running", job.command).ok();
        }
    }

    //keep only the jobs still running
    let all = std::mem::take(jobs);
    for (job, done) in all.into_iter().zip(finished) {
        if !done {
            jobs.push(job);
        }
    }
}

//"+" for the most recent job, "-" for the one before, a space for the others
fn marker(index: usize, count: usize) -> char {
    if index + 1 == count {
        '+'
    } else if index + 2 == count {
        '-'
    } else {
        ' '
    }
}
