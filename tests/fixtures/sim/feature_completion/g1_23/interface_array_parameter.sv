// llg-test-fixture: IEEE 1800-2009 25.3. A generate loop elaborates an array
// of parameterized interface instances. Each element's fixed parameter data
// binds to the intended instance and shape; sibling elements stay distinct.
interface bus_if #(parameter int W = 8, parameter logic [W-1:0] ID = 0);
    logic [W-1:0] tag;
    assign tag = ID;
    modport view (input tag);
endinterface

module reader(bus_if.view v, output logic [7:0] got);
    assign got = v.tag;
endmodule

module tb;
    logic [7:0] got [0:1];
    genvar i;
    generate
        for (i = 0; i < 2; i = i + 1) begin : g
            bus_if #(.W(8), .ID(8'hA0 + i[7:0])) u_if ();
            reader u_r(.v(u_if.view), .got(got[i]));
        end
    endgenerate

    initial begin
        #1 $display("got=%h %h", got[0], got[1]);
        $finish(0);
    end
endmodule
