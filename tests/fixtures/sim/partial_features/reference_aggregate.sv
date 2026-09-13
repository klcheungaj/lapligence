// llg-test-fixture: tests/fixtures/sim/partial_features/reference_aggregate.sv
// LRM: IEEE 1800-2009 23.2.2.2 and 23.3.3
typedef struct {
    logic [3:0] value;
    logic flag;
    string label;
} payload_t;

module aggregate_leaf(ref payload_t p);
    initial begin
        #1 p.value = 4'ha;
        p.flag = 1'b1;
        p.label = "ok";
    end
endmodule

module aggregate_middle(ref payload_t p);
    aggregate_leaf l(p);
endmodule

module tb;
    payload_t p;
    aggregate_middle m(p);
    initial begin
        p.value = 4'h0;
        p.flag = 1'b0;
        p.label = "";
        #2 $display("aggregate %h %b %s", p.value, p.flag, p.label);
        $finish(0);
    end
endmodule
