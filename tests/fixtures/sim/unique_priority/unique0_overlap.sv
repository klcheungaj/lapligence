// llg-test-fixture: tests/fixtures/sim/unique_priority/unique0_overlap.sv
module tb;
    logic [1:0] sel;
    initial begin
        sel = 2'd1;
        unique0 case (sel)
            2'd1: $display("unique0-first");
            2'd1: $display("unique0-second");
        endcase
        $finish;
    end
endmodule
