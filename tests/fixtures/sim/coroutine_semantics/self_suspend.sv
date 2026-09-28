// llg-test-fixture: tests/fixtures/sim/coroutine_semantics/self_suspend.sv
// Suspending process::self records the continuation until another process
// resumes it. Calling resume on the running process is a harmless no-op.
module tb;
    process worker;
    event worker_ready;
    integer stage = 0;
    integer suspended_status = -1;
    integer self_resume_status = -1;

    initial begin
        worker = process::self();
        stage = 1;
        #0;
        -> worker_ready;
        worker.suspend();
        stage = 2;
        worker.resume();
        self_resume_status = worker.status();
    end

    initial begin
        @worker_ready;
        #0;
        suspended_status = worker.status();
        if (stage != 1 || suspended_status != 3)
            $fatal(1, "self suspension mismatch");
        worker.resume();
        worker.await();
        if (stage != 2)
            $fatal(1, "self resume mismatch");
        $display("PASS self_suspend suspended=%0d before=1 after=%0d self_resume=%0d",
            suspended_status, stage, self_resume_status);
        $finish(0);
    end
endmodule
