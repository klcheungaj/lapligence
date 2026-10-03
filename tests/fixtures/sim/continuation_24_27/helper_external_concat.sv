// llg-test-fixture: one external leaf makes a composite helper write visible;
// the waiting process evaluates it and the external write is published.
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
    initial begin
        changes=0; source=0;
        #1 changes=0; source=16'h1234;
        #1 $display("changes=%0d external=%h", changes, external_value);
        $finish(0);
    end
endmodule
