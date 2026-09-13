module child(input wire en, inout wire [7:0] bus);
    assign bus[0] = en;
endmodule
module tb;
    wire [7:0] bus;
    reg en;
    child u(.en(en), .bus(bus));
    initial begin
        en = 1;
        #1;
        $display("bus=%h", bus);
        $finish(0);
    end
endmodule
