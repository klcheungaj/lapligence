// Decision A18-D2: when the simulation ends while a DPI-C chain is
// suspended in an exported task, the chain never resumes: the exported task
// does not return to its C caller, and no foreign code of that chain runs
// after the simulation ended. The final procedures run and the run ends
// normally.
//
// IEEE 1800-2009 20.2 (SystemVerilog-1800-2009.txt L34119):
//   "The $finish system task causes the simulator to exit and pass control
//   back to the host operating system."
// 35.5.1.5 (L55696-55698): "A call to an imported task can result in the
//   suspension of the currently executing thread. This occurs when an
//   imported task calls an exported task, and the exported task executes a
//   delay control, event control, or wait statement."
// The text does not say whether suspended foreign code resumes at the end
// of the simulation; $finish is not a disable (35.9), so llg resumes
// nothing.
//
// Build A18-D2_finish_leaves_chain_suspended.c into a shared library and
// load it with the simulator's DPI library option.
`timescale 1ns/1ns
module tb;
    import "DPI-C" context task a2_task();
    import "DPI-C" function int a2_resumed();
    export "DPI-C" task a2_wait;

    task automatic a2_wait();
        #100;
    endtask

    initial begin
        $display("call at %0t", $time);
        a2_task();
        $display("not reached");
    end

    initial #10 begin
        $display("finish at %0t", $time);
        $finish;
    end

    final $display("final resumed=%0d", a2_resumed());
endmodule
