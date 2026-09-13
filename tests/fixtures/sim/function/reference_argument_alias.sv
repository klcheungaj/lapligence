module tb;
    logic [7:0] value;
    logic bits [7:0];
    logic [7:0] memory [0:1];
    integer selector_calls;

    function automatic void update_two(ref logic [7:0] first, ref logic [7:0] second);
        first = 8'd9;
        $display("alias-inside=%0d", second);
    endfunction

    function automatic logic [7:0] read_const_inner(const ref logic [7:0] source);
        read_const_inner = source;
    endfunction

    function automatic logic [7:0] read_ref(const ref logic [7:0] source);
        read_ref = read_const_inner(source);
    endfunction

    function automatic void update_nested(ref logic [7:0] source);
        source = read_ref(source) + 8'd1;
    endfunction

    function automatic void set_bit(ref logic bit_value);
        bit_value = 1'b1;
    endfunction

    function automatic void set_element(ref logic [7:0] element);
        element = 8'haa;
    endfunction

    function automatic logic [31:0] next_slot();
        selector_calls = selector_calls + 1;
        next_slot = 32'd1;
    endfunction

    function automatic void recurse(ref logic [7:0] source, input logic [7:0] count);
        if (count == 0)
            source = source + 8'd1;
        else
            recurse(source, count - 8'd1);
    endfunction

    task automatic observe(const ref logic [7:0] source);
        #2;
        $display("const-after=%0d", source);
    endtask

    initial begin
        value = 8'd1;
        update_two(value, value);
        $display("alias-after=%0d", value);
        update_nested(value);
        $display("nested-after=%0d", value);
        foreach (bits[i]) bits[i] = 0;
        // An unpacked array element is a legal ref actual in IEEE 1800-2009.
        set_bit(bits[2]);
        $display("bit-after=%0d", {bits[7], bits[6], bits[5], bits[4],
                                  bits[3], bits[2], bits[1], bits[0]});
        memory[1] = 8'd0;
        set_element(memory[1]);
        $display("array-after=%0d", memory[1]);
        memory[1] = 8'd0;
        set_element(memory[next_slot()]);
        $display("dynamic-array-after=%0d calls=%0d", memory[1], selector_calls);
        recurse(value, 8'd2);
        $display("recursive-after=%0d", value);
        observe(value);
        $display("done");
        $finish(0);
    end

    initial begin
        #1 value = 8'd42;
    end
endmodule
