// IEEE 1800-2009 7.3.2: the largest finite payload below the packed value
// capacity (131,071 bytes plus one tag bit = 1,048,569 bits) keeps one
// owner; element selects with runtime indices address it directly.
typedef union tagged { void Empty; logic [7:0] Table [0:131070]; } item_t;

module tb;
    item_t value;
    int i;
    initial begin
        value = tagged Empty;
        if (value matches tagged Empty) $display("empty");
        value = tagged Table ('{default: 8'h00});
        i = 131070;
        value.Table[i] = 8'h7f;
        value.Table[0] = 8'h01;
        $display("first=%h last=%h middle=%h", value.Table[0], value.Table[i],
                 value.Table[65535]);
        $finish(0);
    end
endmodule
