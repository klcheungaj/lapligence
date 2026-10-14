// Decision S41-D6: an exported subroutine called from a noncontext import is
// a fatal simulation error at that call; the simulation stops after the
// output before it.
//
// IEEE 1800-2009 35.5.3 (SystemVerilog-1800-2009.txt L55796-55797):
//   "Since a noncontext imported DPI subroutine cannot make a call to a
//   SystemVerilog export subroutine, the behavior of making any such calls in
//   the DPI import call chain is an error."
// H.9 (L72543-72545): "the behavior of exported subroutines is undefined when
//   they are invoked by a DPI call chain that lacks the context
//   characteristic."
// The text calls it an error without saying how it is reported; llg makes it
// fatal, as 35.9 does for the disable protocol.
//
// Expected: the first line only, then a fatal error (exit status nonzero).
// Build S41-D6_noncontext_export_call.c into a shared library and load it
// with the simulator's DPI library option.
module tb;
    import "DPI-C" function int d6_plain(input int n);
    export "DPI-C" function d6_inc;

    function int d6_inc(input int n);
        return n + 1;
    endfunction

    int r = 0;

    initial begin
        $display("before");
        r = d6_plain(1);
        $display("after r=%0d", r);
        $finish;
    end
endmodule
