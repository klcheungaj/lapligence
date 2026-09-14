// R27: strobe/monitor alias reads cannot perform a signal write.
module tb;
    logic driver = 0;
    wire a, b;
    alias a = b;
    assign a = driver;
    initial begin
        #1;
        $strobe("strobe=%b%b", a, b);
        driver <= 1;
        #1;
        $monitor("monitor=%b%b", a, b);
        driver <= 0;
        #1;
        $monitoroff;
        $finish(0);
    end
endmodule
