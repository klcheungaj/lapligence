// llg-test-fixture: tests/fixtures/sim/unique_priority/overlap.sv
module tb;
    logic [1:0] sel;
    initial begin
        sel = 2'd1;
        unique case (sel)
            2'd1: $display("first");
            2'd1: $display("second");
        endcase
        $finish;
    end
endmodule
