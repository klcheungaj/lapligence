// llg-test-fixture: tests/fixtures/sim/unique_priority/ladder.sv
module tb;
    logic first;
    logic second;
    initial begin
        first = 1'bx;
        second = 1'b0;
        unique if (first)
            $display("first");
        else if (second)
            $display("second");
        $finish;
    end
endmodule
