module tb;
    logic [127:0] source = 7;
    logic [127:0] saved;
    task automatic delayed(input logic [127:0] value, output logic [127:0] target);
        #1;
        target = value * 3 + 2;
    endtask
    initial begin
        saved = #2 source;
        $display("snapshot=%0d,source=%0d", saved, source);
        delayed(source, saved);
        $display("task=%0d", saved);
        $finish(0);
    end
    initial begin
        #1 source = 9;
    end
endmodule
