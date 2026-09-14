// llg-test-fixture: tests/fixtures/sim/review_batch4/packed_prefixes.sv
module tb;
    logic a = 0, b = 0, c = 0;
    logic [2:0] q;
    logic [1:0] copy;
    logic [4:5] ascending;
    logic [1:0][1:0] matrix;
    typedef struct packed { logic high; logic low; } pair_t;
    pair_t fields;
    function automatic void field_low(input logic value);
        fields.low = value;
    endfunction
    always_comb field_low(a);
    always_comb fields.high = b;
    function automatic void top_bit(ref logic [2:0] target, input logic value);
        target[2] = value;
    endfunction
    always_comb q[0] = a;
    always_comb q[1] = b;
    always_comb top_bit(q, c);
    always_comb copy[0] = copy[1];
    always_comb copy[1] = b;
    always_comb ascending[4] = a;
    always_comb ascending[5] = b;
    always_comb matrix[0] = {a, b};
    always_comb matrix[1] = {b, c};
    initial begin
        #1;
        if (q !== 3'b000 || copy !== 2'b00 || ascending !== 2'b00 || matrix !== 4'b0000)
            $fatal(1, "prefix initialization");
        a = 1; c = 1;
        #1;
        if (fields !== 2'b01 || q !== 3'b101 || ascending !== 2'b10 || matrix !== 4'b0110) $fatal(1, "disjoint packed prefixes");
        b = 1;
        #1;
        if (fields !== 2'b11 || q !== 3'b111 || copy !== 2'b11 || ascending !== 2'b11 || matrix !== 4'b1111)
            $fatal(1, "read slice excluded with different write slice");
        $display("packed prefix dependencies ok");
        $finish(0);
    end
endmodule
