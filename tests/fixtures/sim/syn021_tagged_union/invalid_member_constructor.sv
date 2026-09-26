// IEEE 1800-2009 §11.9: the constructor member must belong to its context type.
typedef union tagged packed {
    logic [7:0] good;
    logic [7:0] other;
} item_t;

module tb;
    item_t value;
    initial value = tagged missing(8'h12);
endmodule
