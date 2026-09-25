// llg-test-fixture: tests/fixtures/sim/review_library_precedence/top.sv
module top;
    wire [7:0] value;
    cell_body instance_name(value);
    initial begin
        #1;
        $display("mapped=%0d", value);
        $finish(0);
    end
endmodule
