// IEEE 1800-2009 §11.9: a void member has no value expression.
typedef union tagged packed {
    void empty;
    logic [7:0] data;
} item_t;

module tb;
    item_t value;
    initial value = tagged empty(8'h12);
endmodule
