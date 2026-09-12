// llg-test-fixture: tests/fixtures/sim/unique_priority/if_overlap.sv
module tb;
    logic first;
    logic second;
    initial begin
        first = 1'b1;
        second = 1'b1;
        unique if (first)
            $display("first");
        else if (second)
            $display("second");
        $finish;
    end
endmodule
