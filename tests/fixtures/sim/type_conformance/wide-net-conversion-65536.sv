module tb;
    localparam W=65536;
    typedef bit [W-1:0] two_t;
    logic [W-1:0] source;
    reg [W-1:0] copied;
    two_t two, expected;
    uwire [W-1:0] net_value=source;
    initial begin
        source='0; source[W-1]=1; source[W-2]=1'bx;
        source[65]=1'bz; source[64]=1; source[0]=1'bx;
        expected='0; expected[W-1]=1; expected[64]=1;
        #1;
        if (net_value !== source) $display("FAIL wide net state");
        two=net_value; copied=two;
        if (two !== expected || copied !== expected || two_t'(net_value) !== expected)
            $display("FAIL wide conversion");
        source='z; #1;
        if (net_value !== 'z || two_t'(net_value) !== '0) $display("FAIL wide release");
        $display("PASS wide net conversion"); $finish(0);
    end
endmodule
