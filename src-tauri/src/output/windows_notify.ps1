try {
    Add-Type -AssemblyName PresentationFramework
    Add-Type -AssemblyName PresentationCore
    Add-Type -AssemblyName WindowsBase

    $xaml = @"
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        WindowStyle="None" AllowsTransparency="True"
        Background="#CC2D2D2D" Opacity="0.92"
        ShowInTaskbar="False" Topmost="True"
        SizeToContent="Height" Width="320"
        WindowStartupLocation="Manual">
    <Border CornerRadius="12" Padding="16,12" Background="#CC2D2D2D"
            BorderBrush="#44FFFFFF" BorderThickness="1">
        <StackPanel>
            <TextBlock Foreground="#CCFFFFFF"
                       FontSize="13" FontWeight="SemiBold" Margin="0,0,0,4"/>
            <TextBlock Foreground="#AAFFFFFF"
                       FontSize="12" TextWrapping="Wrap"/>
        </StackPanel>
    </Border>
</Window>
"@

    $reader = [System.Xml.XmlReader]::Create([System.IO.StringReader]::new($xaml))
    $window = [System.Windows.Markup.XamlReader]::Load($reader)

    $window.Content.Child.Children[0].Text = '{{title}}'
    $window.Content.Child.Children[1].Text = '{{body}}'

    $screen = [System.Windows.SystemParameters]::WorkArea
    $window.Left = $screen.Right - $window.Width - 24
    $window.Top = $screen.Bottom - $window.Height - 24

    $timer = New-Object System.Windows.Threading.DispatcherTimer
    $timer.Interval = [TimeSpan]::FromSeconds({{timeout_sec}})
    $timer.Add_Tick({ $window.Close(); $timer.Stop() })
    $timer.Start()

    $window.ShowDialog() | Out-Null
} catch {
    Add-Type -AssemblyName System.Windows.Forms
    [System.Windows.Forms.MessageBox]::Show("{{body}}", "{{title}}", "OK", "Information")
}
