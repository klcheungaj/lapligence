// IEEE 1800-2009 7.3.2: a legal finite unpacked tagged union whose payload
// exceeds the packed value capacity. Descriptor transport covers integral
// arrays only, so the union is rejected rather than flattened.
typedef union tagged { void Empty; logic [7:0] Table [0:131071]; } item_t;

module tb;
    item_t value;
    initial value = tagged Empty;
endmodule
