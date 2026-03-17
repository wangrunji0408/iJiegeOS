/// Linux信号定义
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Signal {
    SIGHUP  = 1,
    SIGINT  = 2,
    SIGQUIT = 3,
    SIGILL  = 4,
    SIGTRAP = 5,
    SIGABRT = 6,
    SIGBUS  = 7,
    SIGFPE  = 8,
    SIGKILL = 9,
    SIGUSR1 = 10,
    SIGSEGV = 11,
    SIGUSR2 = 12,
    SIGPIPE = 13,
    SIGALRM = 14,
    SIGTERM = 15,
    SIGSTKFLT = 16,
    SIGCHLD = 17,
    SIGCONT = 18,
    SIGSTOP = 19,
    SIGTSTP = 20,
    SIGTTIN = 21,
    SIGTTOU = 22,
    SIGURG  = 23,
    SIGXCPU = 24,
    SIGXFSZ = 25,
    SIGVTALRM = 26,
    SIGPROF = 27,
    SIGWINCH = 28,
    SIGIO   = 29,
    SIGPWR  = 30,
    SIGSYS  = 31,
}

impl Signal {
    pub fn from_num(n: u32) -> Option<Self> {
        match n {
            1  => Some(Self::SIGHUP),
            2  => Some(Self::SIGINT),
            3  => Some(Self::SIGQUIT),
            4  => Some(Self::SIGILL),
            5  => Some(Self::SIGTRAP),
            6  => Some(Self::SIGABRT),
            7  => Some(Self::SIGBUS),
            8  => Some(Self::SIGFPE),
            9  => Some(Self::SIGKILL),
            10 => Some(Self::SIGUSR1),
            11 => Some(Self::SIGSEGV),
            12 => Some(Self::SIGUSR2),
            13 => Some(Self::SIGPIPE),
            14 => Some(Self::SIGALRM),
            15 => Some(Self::SIGTERM),
            17 => Some(Self::SIGCHLD),
            18 => Some(Self::SIGCONT),
            19 => Some(Self::SIGSTOP),
            _  => None,
        }
    }
}
