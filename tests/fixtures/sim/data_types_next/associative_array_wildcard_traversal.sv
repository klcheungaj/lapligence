// IEEE 1800-2009 7.8.1: first/last/next/prev are illegal for wildcard-key
// associative arrays and must be rejected before generated C execution.
module tb;
    logic [7:0] values[*];
    integer key;
    integer status;

    initial begin
        status = values.first(key);
        $display("UNEXPECTED status=%0d key=%0d", status, key);
        $finish;
    end
endmodule
