// IEEE 1800-2009 7.3 and 7.3.2: an ordinary unpacked union is one storage
// object that may be written through one member and read through another.
module tb;
    typedef union {
        logic [31:0] first;
        logic [31:0] second;
    } unpacked_union_t;

    unpacked_union_t value;

    initial begin
        if (value.first !== 32'hxxxxxxxx) begin
            $display("FAIL unpacked_union default");
            $finish;
        end

        value.first = 32'hdeadbeef;
        if (value.second !== 32'hdeadbeef) begin
            $display("FAIL unpacked_union first_to_second");
            $finish;
        end
        value.second = 32'h12345678;
        if (value.first !== 32'h12345678) begin
            $display("FAIL unpacked_union second_to_first");
            $finish;
        end

        $display("PASS unpacked_union");
        $finish;
    end
endmodule
