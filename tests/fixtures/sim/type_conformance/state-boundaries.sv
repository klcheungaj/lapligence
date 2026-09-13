module pass(input bit [128:0] a, output bit [128:0] b);
    assign b=a;
endmodule
module tb;
    logic [128:0] source, expected, result, task_result;
    wire [128:0] port_result;
    bit [128:0] value;
    pass dut(source,port_result);
    function automatic bit [128:0] from_four(input logic [128:0] a);
        from_four=a;
    endfunction
    function automatic logic [128:0] from_two(input bit [128:0] a);
        from_two=a;
    endfunction
    task automatic through_task(input logic [128:0] a, output bit [128:0] b);
        b=a;
    endtask
    initial begin
        source='0; source[128]=1; source[65]=1'bx; source[64]=1'bz;
        source[2]=1'bx; source[1]=1'bz; source[0]=1;
        expected='0; expected[128]=1; expected[0]=1;
        value<=source;
        result=from_four(source); through_task(source,task_result);
        #1;
        if (value !== expected || port_result !== expected || dut.a !== expected ||
            result !== expected || task_result !== expected || from_two(source) !== expected) $display("FAIL boundary conversion");
        value='1; value[65 -: 4]<=source[65 -: 4];
        expected='1; expected[65 -: 4]=4'b0000;
        #1;
        if (value !== expected) $display("FAIL selected NBA");
        $display("PASS state boundaries"); $finish(0);
    end
endmodule
