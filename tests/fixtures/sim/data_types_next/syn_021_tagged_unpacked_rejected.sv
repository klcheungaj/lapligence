// IEEE 1800-2009 §7.3.2 and §11.9: this single negative case keeps the
// finite tagged representation boundary explicit by omitting `packed`.
typedef union tagged {
    void empty;
    logic [7:0] narrow;
} tagged_t;

module tb;
    tagged_t value;

    initial value = tagged narrow(8'h5a);
endmodule
