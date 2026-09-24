// llg-test-fixture: IEEE 1800-2009 §§6.24, 7.2, 9.2.2.2, 10.4.2, and 13.
// The packed-struct cast result has a function return slot and is consumed
// only as the NBA RHS of this clocked process.
typedef struct packed {
    logic [7:0] high;
    logic [7:0] low;
} pair_t;

module tb;
    logic clk;
    pair_t registered;

    function automatic pair_t make_pair(input logic [15:0] bits);
        return pair_t'(bits);
    endfunction

    always_ff @(posedge clk)
        registered <= make_pair(16'h1234);

    initial begin
        clk = 1'b0;
        #1;
        clk = 1'b1;
        #1;
        if (registered.high !== 8'h12 || registered.low !== 8'h34)
            $fatal(1, "packed cast return in always_ff stored the wrong fields");
        $display("%h %h", registered.high, registered.low);
        $finish(0);
    end
endmodule
