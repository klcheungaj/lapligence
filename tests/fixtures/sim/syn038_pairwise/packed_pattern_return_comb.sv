// llg-test-fixture: IEEE 1800-2009 §§10.9.1, 13, and 9.2.2.2.
// An automatic function returns a complete packed-byte positional pattern;
// its only call site is a module always_comb assignment.
module tb;
    logic [7:0] source;
    logic [7:0] result;

    function automatic logic [7:0] make_byte(input logic [7:0] bits);
        return '{bits[7], bits[6], bits[5], bits[4],
                 bits[3], bits[2], bits[1], bits[0]};
    endfunction

    always_comb result = make_byte(source);

    initial begin
        source = 8'ha5;
        #1;
        if (result !== 8'ha5)
            $fatal(1, "packed pattern function return changed the byte");
        $display("result=%h", result);
        $finish(0);
    end
endmodule
