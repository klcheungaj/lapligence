// SIM-041: SV -> C -> SV recursion through a context import and an export
// uses the native stack for every level and stops at the simulator's
// recursion limit (256 subroutine activations): 40 levels return their
// count; 1000 levels report the limit and the run continues.
module tb;
    import "DPI-C" context function int rc_down(input int n);
    export "DPI-C" function rc_up;

    function automatic int rc_up(input int n);
        return n <= 0 ? 0 : 1 + rc_down(n - 1);
    endfunction

    int r;

    initial begin
        r = rc_down(40);
        $display("depth 40: %0d", r);
        r = rc_down(1000);
        $display("depth 1000 returned");
        $finish;
    end
endmodule
