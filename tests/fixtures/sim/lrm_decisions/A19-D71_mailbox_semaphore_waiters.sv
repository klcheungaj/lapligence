// Decision A19-D71: the class object of a mailbox or semaphore variable
// lists, under vpiWaitingProcesses, the threads (vpiThread) suspended in a
// get, peek or put of the mailbox or a get of the semaphore, and drops each
// one once it resumes. The object is named by its built-in class (mailbox,
// semaphore), as decision A19-D64 names a class object by its class.
//
// IEEE 1800-2009 37.29 detail 3 (SystemVerilog-1800-2009.txt L59615-59617):
// "The vpiWaitingProcesses iterator on a mailbox or semaphore shall return
// the threads waiting on the class object or object resource. A waiting
// process is a static or dynamic process represented by its suspended
// thread. A process may be waiting to retrieve a message from a mailbox or
// waiting for a semaphore resource key." A put blocked on a full bounded
// mailbox (15.4.3) waits on the mailbox too, so it is listed. Some
// simulators list a waiting process as a vpiFrame, leave out the puts
// waiting on a full mailbox and name the object after its specialization
// (mailbox#(int)); llg follows the clause text, which returns the threads
// waiting on the object, and names it by its built-in class.
//
// Build A19-D71_mailbox_semaphore_waiters.c as a VPI application (llg:
// static archive given to --dpi-lib; `task $d71()` declared as a system
// task).
module tb;
    mailbox #(int) mb = new;
    mailbox #(int) full = new(1);
    semaphore sm = new(0);
    initial begin
        int x;
        void'(full.try_put(7));
        fork
            begin mb.get(x); $display("got %0d", x); end
            begin int y; #0 mb.get(y); $display("second got %0d", y); end
            begin full.put(8); $display("put into full"); end
            begin sm.get(2); $display("got keys"); end
        join_none
        #1 $d71;
        mb.put(5);
        sm.put(2);
        #1 $d71;
        $finish;
    end
endmodule
