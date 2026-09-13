// llg-test-fixture: tests/fixtures/sim/process_semantics/static_prefix.sv
// IEEE 1800-2009 Sections 7.4, 9.2.2.2 and 13.4: independent fixed-array
// elements have independent writers, while called functions contribute their
// array reads and side effects to always_comb semantics.
module tb;
    logic [7:0] mem [0:1];
    logic [7:0] a;
    logic [7:0] b;
    logic index;
    logic [7:0] selected;
    logic [7:0] result;
    logic helper;

    function automatic logic [7:0] pick(input logic which);
        pick = mem[which];
    endfunction

    function automatic logic [7:0] side_effect(input logic value);
        helper = value;
        side_effect = value;
    endfunction

    always_comb mem[0] = a;
    always_comb mem[1] = b;
    always_comb selected = pick(index);
    always_comb result = side_effect(a);

    initial begin
        a = 8'h11;
        b = 8'h22;
        index = 1'b0;
        #1 $display("initial selected=%h result=%h m0=%h m1=%h helper=%b", selected, result, mem[0], mem[1], helper);
        index = 1'b1;
        #1 $display("switch selected=%h result=%h m0=%h m1=%h helper=%b", selected, result, mem[0], mem[1], helper);
        a = 8'h33;
        #1 $display("change selected=%h result=%h m0=%h m1=%h helper=%b", selected, result, mem[0], mem[1], helper);
        $finish(0);
    end
endmodule
