// llg-test-fixture: tests/fixtures/sim/feature_completion/g1_05/owner_cancel_unwind.sv
// G1-05 owner_cancel_unwind: disabling a suspended owner and $finish with a
// second live detached owner must release every wide temporary exactly once
// and must not commit a disabled branch's value. IEEE 1800-2009 9.6.2, 9.6.3.
module tb;
    logic [129:0] shared;

    initial begin
        shared = 130'd1;
        fork : victim
            begin
                logic [129:0] temporary;
                temporary = shared + 130'd41;
                #5;
                shared = temporary;
            end
        join_none
        fork : tail
            begin
                logic [129:0] pending;
                pending = shared + 130'd100;
                #100;
                shared = pending;
            end
        join_none
        #1;
        disable victim;
        $display("shared=%0d", shared);
        #1;
        $finish(0);
    end
endmodule
