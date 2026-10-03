// SV2009 §§7.4.6, 10.4, 11.5, 23.2.2: stable large-array refs and resolved ranges.
module leaf(ref logic [31:0] value);
    initial begin
        #1;
        value[31 -: 3] = 3'b101;
        value[-1 +: 3] = 3'b111;
        #2;
        if (value !== 32'h89abcdef) $fatal;
        value[7:4] <= 4'h5;
        #1;
        if (value !== 32'h89abcd5f) $fatal;
    end
endmodule
module middle(ref logic [64:0] value);
    leaf child(value[64:33]);
endmodule
module tb;
    logic [128:0] values [0:16777215], source [0:16777215];
    logic conflict = 0;
    wire [128:0] resolved [0:3];
    middle child(values[16777215][80:16]);
    for (genvar element = 0; element < 4; element++) begin : connections
        assign resolved[element][128:64] = values[16777215][128:64];
        assign resolved[element][63:0] = values[0][63:0];
        assign resolved[element][80:78] = conflict ? 3'b000 : 3'bzzz;
    end
    initial begin
        values[16777215] = 0;
        values[0] = 7;
        source[16777215] = 0;
        source[0] = 7;
        #2;
        if (values[16777215][80:78] !== 3'b101 ||
            values[16777215][50:49] !== 2'b11 || values[16777215][48:0] !== 0) $fatal;
        source[16777215][80:49] = 32'h89abcdef;
        values = source;
        #3;
        if (values[16777215][80:49] !== 32'h89abcd5f || values[1] !== 129'bx) $fatal;
        for (int element = 0; element < 4; element++) begin
            if (resolved[element][80:78] !== 3'b100 || resolved[element][63:0] !== 7) $fatal;
        end
        conflict = 1;
        #1;
        for (int element = 0; element < 4; element++) begin
            if (resolved[element][80:78] !== 3'bx00 || resolved[element][63:0] !== 7) $fatal;
        end
        $display("PASS rtl002 integration views");
        $finish(0);
    end
endmodule
