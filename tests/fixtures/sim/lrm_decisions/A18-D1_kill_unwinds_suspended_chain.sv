// Decision A18-D1: killing a process whose DPI-C chain is suspended in an
// exported task unwinds the chain at the kill through the disable protocol:
// the exported task returns 1, svIsDisabledState() is 1 in the import's C
// code, which must return 1, and the import's outputs are not copied back.
//
// IEEE 1800-2009 9.7 (SystemVerilog-1800-2009.txt L12631-12632):
//   "The kill() function terminates the given process and all its
//   subprocesses, that is, processes spawned using fork statements by the
//   process being killed."
// 35.9 (L56262-56264): "An imported subroutine is said to be in the disabled
//   state when a disable statement somewhere in the design targets either it
//   or a parent for disabling. An imported subroutine can only enter the
//   disabled state immediately after the return of a call to an exported
//   subroutine."
// 35.9 a) (L56270-56271): "When an exported task returns due to a disable,
//   it shall return a value of 1."
// The text does not say what a kill does to suspended foreign code; llg
// treats it as a disable of the killed process, so the C code can clean up
// (35.9 L56258-56260) before the kill returns.
//
// Build A18-D1_kill_unwinds_suspended_chain.c into a shared library and load
// it with the simulator's DPI library option.
`timescale 1ns/1ns
module tb;
    import "DPI-C" context task a1_task(output int o);
    import "DPI-C" function int a1_seen();
    export "DPI-C" task a1_wait;

    process p;
    int o = 7;

    task automatic a1_wait();
        #100;
    endtask

    initial begin
        p = process::self();
        $display("call at %0t", $time);
        a1_task(o);
        $display("not reached");
    end

    initial begin
        #10;
        p.kill();
        $display("after kill at %0t seen=%0d o=%0d", $time, a1_seen(), o);
        $finish;
    end
endmodule
