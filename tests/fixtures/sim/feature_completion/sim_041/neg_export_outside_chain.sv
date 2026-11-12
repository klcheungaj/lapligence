// SIM-041 negative: foreign code calls an export outside any DPI-C import
// call chain. outside.c, linked as a VPI application, calls the export from
// the calltf of $outside (SV 35.5.3: only a context import call chain may
// call exported subroutines). Run with the prototype in sim_041.rs.
module tb;
    export "DPI-C" function err_inc;

    function int err_inc(input int n);
        return n + 1;
    endfunction

    initial begin
        $display("start");
        $outside;
        $display("not reached");
        $finish;
    end
endmodule
