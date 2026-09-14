// R03: a blocking mismatch is an error, not an indefinitely blocked read.
module tb;
    mailbox box = new;
    string text = "unchanged";
    initial begin
        box.put(42);
        box.get(text);
        $display("UNREACHABLE");
    end
    initial begin #2; $display("TIMEOUT"); $finish(0); end
endmodule
