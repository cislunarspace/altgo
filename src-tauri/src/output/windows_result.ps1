try {
    Add-Type -AssemblyName PresentationFramework
    Add-Type -AssemblyName PresentationCore
    Add-Type -AssemblyName WindowsBase

    $xaml = @"
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        WindowStyle="None" AllowsTransparency="True"
        Background="#CC1E1E1E" Opacity="0.95"
        ShowInTaskbar="False" Topmost="True"
        SizeToContent="Height" Width="420"
        WindowStartupLocation="Manual"
        Left="{{left}}" Top="{{top}}">
    <Border CornerRadius="16" Padding="20,16" Background="#CC1E1E1E"
            BorderBrush="#33FFFFFF" BorderThickness="1">
        <StackPanel>
            <!-- Title row -->
            <Grid Margin="0,0,0,12">
                <TextBlock Text="{{title}}" Foreground="#CCCCCC"
                           FontSize="14" FontWeight="SemiBold" VerticalAlignment="Center"/>
                <Button Content="X" HorizontalAlignment="Right"
                        Background="Transparent" Foreground="#666666"
                        BorderThickness="0" FontSize="13" Padding="6,2"
                        Cursor="Hand" x:Name="CloseBtn"/>
            </Grid>

            <!-- Raw text section -->
            <TextBlock Text="原始识别" Foreground="#888888" FontSize="11" Margin="0,0,0,4"/>
            <Border Background="#2A2A2A" CornerRadius="8" Padding="10,8" Margin="0,0,0,4"
                    MaxHeight="80">
                <ScrollViewer VerticalScrollBarVisibility="Auto" HorizontalScrollBarVisibility="Disabled">
                    <TextBlock Text="{{raw_text}}" Foreground="#AAAAAA"
                               FontSize="12" TextWrapping="Wrap"/>
                </ScrollViewer>
            </Border>

            <!-- Polished text section -->
            <TextBlock Text="润色结果" Foreground="#888888" FontSize="11" Margin="0,0,0,4"/>
            <Border Background="#2A2A2A" CornerRadius="8" Padding="10,8" Margin="0,0,0,12"
                    MaxHeight="80">
                <ScrollViewer VerticalScrollBarVisibility="Auto" HorizontalScrollBarVisibility="Disabled">
                    <TextBlock Text="{{polished_text}}" Foreground="#DDDDDD"
                               FontSize="12" TextWrapping="Wrap"/>
                </ScrollViewer>
            </Border>

            <!-- Copy button -->
            <Button Content="复制" Padding="0,8" Background="#3A3A3A"
                    Foreground="#CCCCCC" BorderThickness="0"
                    FontSize="13" Cursor="Hand" HorizontalAlignment="Stretch"
                    x:Name="CopyBtn"/>
        </StackPanel>
    </Border>
</Window>
"@

    $reader = [System.Xml.XmlReader]::Create([System.IO.StringReader]::new($xaml))
    $window = [System.Windows.Markup.XamlReader]::Load($reader)

    # Wire up buttons
    $window.FindName("CopyBtn").Add_Click({
        Set-Clipboard '{{polished_text}}'
        $window.Close()
    })
    $window.FindName("CloseBtn").Add_Click({ $window.Close() })

    # Auto-dismiss timer
    $timer = New-Object System.Windows.Threading.DispatcherTimer
    $timer.Interval = [TimeSpan]::FromMilliseconds({{timeout_ms}})
    $timer.Add_Tick({ $window.Close(); $timer.Stop() })
    $timer.Start()

    $window.ShowDialog() | Out-Null
} catch {
    Add-Type -AssemblyName System.Windows.Forms
    [System.Windows.Forms.MessageBox]::Show("{{polished_text}}", "altgo result", "OK", "Information")
}
