// R03: the compatible second waiter must not bypass the mismatched first.
module tb;
    mailbox box = new;
    string text;
    int value;
    initial begin box.peek(text); $display("BAD_FIRST"); end
    initial begin #1; box.get(value); $display("BYPASSED"); end
    initial begin #2; box.put(42); $display("BAD_PRODUCER"); end
    initial begin #4; $display("TIMEOUT"); $finish(0); end
endmodule
