// llg-test-fixture: tests/fixtures/sim/stochastic/queue_statistics.sv
`timescale 1ns/1ns
module tb;
    integer fifo;
    integer lifo;
    integer status;
    integer job;
    integer info;
    integer stat;
    integer full;

    initial begin
        fifo = 7;
        lifo = 8;

        $q_initialize(fifo, 1, 2, status);
        $display("init=%0d", status);
        $q_initialize(99, 3, 2, status);
        $display("bad_type=%0d", status);
        $q_initialize(98, 1, 0, status);
        $display("bad_length=%0d", status);
        $q_initialize(fifo, 2, 2, status);
        $display("duplicate=%0d", status);

        job = 77;
        info = 88;
        $q_add(fifo, 1, 101, status);
        $display("add0=%0d", status);
        #5;
        $q_add(fifo, 2, 202, status);
        full = $q_full(fifo, status);
        $display("add5=%0d full=%0d full_status=%0d", status, full, status);
        $q_add(fifo, 3, 303, status);
        $display("add_full=%0d", status);

        #5;
        $q_remove(fifo, job, info, status);
        $display("remove_fifo=%0d job=%0d info=%0d", status, job, info);
        $q_exam(fifo, 1, stat, status);
        $display("stat1=%0d status=%0d", stat, status);
        $q_exam(fifo, 2, stat, status);
        $display("stat2=%0d status=%0d", stat, status);
        $q_exam(fifo, 3, stat, status);
        $display("stat3=%0d status=%0d", stat, status);
        $q_exam(fifo, 4, stat, status);
        $display("stat4=%0d status=%0d", stat, status);
        $q_exam(fifo, 5, stat, status);
        $display("stat5=%0d status=%0d", stat, status);
        $q_exam(fifo, 6, stat, status);
        $display("stat6=%0d status=%0d", stat, status);
        stat = 321;
        $q_exam(fifo, 7, stat, status);
        $display("bad_stat=%0d status=%0d", stat, status);

        #10;
        $q_add(fifo, 3, 303, status);
        #0;
        $q_exam(fifo, 1, stat, status);
        $display("active1=%0d status=%0d", stat, status);
        $q_exam(fifo, 2, stat, status);
        $display("active2=%0d status=%0d", stat, status);
        $q_exam(fifo, 3, stat, status);
        $display("active3=%0d status=%0d", stat, status);
        $q_exam(fifo, 4, stat, status);
        $display("active4=%0d status=%0d", stat, status);
        $q_exam(fifo, 5, stat, status);
        $display("active5=%0d status=%0d", stat, status);
        $q_exam(fifo, 6, stat, status);
        $display("active6=%0d status=%0d", stat, status);

        $q_remove(fifo, job, info, status);
        $q_remove(fifo, job, info, status);
        $q_remove(fifo, job, info, status);
        $display("empty=%0d", status);
        $q_add(99, 1, 1, status);
        $display("unknown_add=%0d", status);
        $q_remove(99, job, info, status);
        $display("unknown_remove=%0d", status);
        full = $q_full(99, status);
        $display("unknown_full=%0d status=%0d", full, status);
        $q_exam(99, 1, stat, status);
        $display("unknown_exam=%0d", status);

        $q_initialize(lifo, 2, 2, status);
        $q_add(lifo, 11, 110, status);
        $q_add(lifo, 12, 120, status);
        $q_remove(lifo, job, info, status);
        $display("remove_lifo1=%0d job=%0d info=%0d", status, job, info);
        $q_remove(lifo, job, info, status);
        $display("remove_lifo2=%0d job=%0d info=%0d", status, job, info);
        $finish;
    end
endmodule
