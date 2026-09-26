// IEEE 1800-2009 §7.3.2: every packed tagged-union member must be packed.
typedef union tagged packed {
    logic [7:0] good;
    logic [7:0] nonpacked [0:1];
} item_t;

module tb;
    item_t value;
endmodule
