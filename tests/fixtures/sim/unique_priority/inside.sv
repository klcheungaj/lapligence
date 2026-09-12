// llg-test-fixture: tests/fixtures/sim/unique_priority/inside.sv
module tb;
    logic [3:0] sel;
    initial begin
        sel = 4'd3;
        unique case (sel) inside
            [4'd1:4'd3]: $display("inside-first");
            [4'd3:4'd4]: $display("inside-second");
            default: $display("inside-default");
        endcase
        $finish;
    end
endmodule
