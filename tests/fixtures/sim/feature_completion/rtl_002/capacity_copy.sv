// IEEE 1800-2009 §§7.4.2, 7.6, 10.4.2: 2^24 cells and independent whole values.
module tb;
    logic [16:0] source [0:16777215];
    logic [16:0] target [0:16777215];
    typedef bit [16:0] converted_t [0:16777215];
    converted_t converted;
    integer invalid;
    initial begin
        source[0] = 17'h12345;
        source[16777215] = 17'h1abcd;
        source[65536] = 17'b1xz01;
        target = source;
        if (target !== source || (target == source) !== 1'bx) $fatal;
        source[0] = 0;
        if (target[0] !== 17'h12345 || target[16777215] !== 17'h1abcd || target[1] !== 17'bx) $fatal;
        converted = converted_t'(target);
        if (converted[65536] !== 17'b10001 || converted[1] !== 0) $fatal;
        target <= source;
        source[16777215] = 0;
        #1;
        if (target[0] !== 0 || target[16777215] !== 17'h1abcd) $fatal;
        target = target;
        invalid = -1;
        target[invalid] = 9;
        invalid = 16777216;
        target[invalid] = 9;
        if (target[0] !== 0 || target[16777215] !== 17'h1abcd) $fatal;
        $display("PASS rtl002 capacity copy");
        $finish(0);
    end
endmodule
