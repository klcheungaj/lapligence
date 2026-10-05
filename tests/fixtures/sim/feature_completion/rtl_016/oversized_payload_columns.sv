// IEEE 1800-2009 7.3.2: a legal finite unpacked tagged union whose payload
// exceeds the packed value capacity. It keeps its tag and each member in
// separate columns (RTL-101) instead of being flattened.
typedef union tagged { void Empty; logic [7:0] Table [0:131071]; } item_t;

module tb;
    item_t value;
    initial begin
        value = tagged Empty;
        value = tagged Table '{default: 8'h5a};
        $display("%h %h", value.Table[0], value.Table[131071]);
        value = tagged Empty;
        if (value matches tagged Empty) $display("empty");
        $finish(0);
    end
endmodule
