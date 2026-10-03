// llg-test-fixture: tests/fixtures/sim/container_selects/compound_select.sv
// A compound assignment to a select of a resizable-container element is not
// lowered yet; it must be rejected with a specific diagnostic rather than
// executing a partial update.
module tb;
    logic [69:0] d[];

    initial begin
        d = new[1];
        d[0][3:0] += 4'h1;
        $display("%h", d[0]);
        $finish;
    end
endmodule
