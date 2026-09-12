// llg-test-fixture: tests/fixtures/sim/unique_priority/wildcards.sv
module tb;
    logic [1:0] sel;
    initial begin
        sel = 2'b1x;
        unique case (sel)
            2'b10: $display("exact");
        endcase
        sel = 2'b10;
        unique casez (sel)
            2'b1?: $display("casez");
        endcase
        sel = 2'b1x;
        unique casex (sel)
            2'b10: $display("casex");
        endcase
        $finish;
    end
endmodule
