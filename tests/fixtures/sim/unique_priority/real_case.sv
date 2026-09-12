// llg-test-fixture: tests/fixtures/sim/unique_priority/real_case.sv
module tb;
    real sel;
    initial begin
        sel = 1.5;
        unique case (sel)
            1.5: $display("real-first");
            1.5: $display("real-second");
        endcase
        $finish;
    end
endmodule
