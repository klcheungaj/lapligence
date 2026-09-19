// IEEE 1800-2009 6.8/6.21/10.5: a legal zero-time function used in a static
// declaration initializer executes before any initial procedure observes the
// variable, including its side effects on other static storage.
module tb;
    int observed;

    function automatic int base();
        observed = 41;
        base = observed + 1;
    endfunction

    int x = base();

    initial begin
        $display("x=%0d observed=%0d", x, observed);
        $finish(0);
    end
endmodule
