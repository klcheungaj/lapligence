// llg-test-fixture: assignment=true on an arithmetic op still denotes a write.
module tb;
    logic [7:0] external_value;
    logic [7:0] source;
    integer changes;
    function automatic logic [7:0] bad(input logic [7:0] value);
        return (external_value += value);
    endfunction
    always @(bad(source)) changes++;
    initial begin source=0; #1; $finish(0); end
endmodule
