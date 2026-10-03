// llg-test-fixture: assignment=true on an arithmetic op still denotes a write;
// the visible compound write is evaluated by the waiting process.
module tb;
    logic [7:0] external_value;
    logic [7:0] source;
    integer changes;
    function automatic logic [7:0] bad(input logic [7:0] value);
        return (external_value += value);
    endfunction
    always @(bad(source)) changes++;
    initial begin
        changes=0; external_value=0; source=0;
        #1 changes=0; source=3;
        #1 $display("changes=%0d accumulated=%0d", changes, external_value >= 3);
        $finish(0);
    end
endmodule
