// llg-test-fixture: tests/fixtures/sim/review_library_configs/top.sv
module tb;
    wire [7:0] first, second;
    selected_leaf u(first);
    selected_leaf v(second);
    initial begin
        #1;
        $display("mapped=%0d %0d", first, second);
        $finish(0);
    end
endmodule
