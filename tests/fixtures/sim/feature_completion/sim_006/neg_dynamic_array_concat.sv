// SV 10.10 allows an array item in a concatenation assigned to a dynamic
// array; only queue targets combine array items, so this rejects explicitly.
module tb;
    int d[];
    initial begin
        d = {1};
        d = {d, 2};
        $display("%0d", d.size());
    end
endmodule
