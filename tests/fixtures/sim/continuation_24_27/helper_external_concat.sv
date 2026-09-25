// llg-test-fixture: one external leaf makes a composite callback write impure.
module tb;
    logic [7:0] external_value;
    logic [15:0] source;
    integer changes;
    function automatic logic [7:0] bad(input logic [15:0] value);
        logic [7:0] private_value;
        {private_value,external_value}=value;
        return private_value;
    endfunction
    always @(bad(source)) changes++;
    initial begin source=0; #1; $finish(0); end
endmodule
