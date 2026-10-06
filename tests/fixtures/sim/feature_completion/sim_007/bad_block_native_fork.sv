// SIM-007 boundary: each loop iteration runs the fork again while the
// `join_none` branch of the previous iteration still reads the fork's
// automatic string (SV 6.21, 9.3.2). One storage per declaration cannot
// keep both activations, so the declaration is rejected rather than shared.
module tb;
    initial begin
        for (int i = 0; i < 2; i++)
            fork
                automatic string name = $sformatf("w%0d", i);
                #1 $display("%s", name);
            join_none
        #5 $finish(0);
    end
endmodule
