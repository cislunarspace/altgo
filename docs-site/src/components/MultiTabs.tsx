import React from 'react';
import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

export default function MultiTabs({children}: {children: React.ReactNode}) {
  const items = React.Children.toArray(children);
  const labels = ['Linux', 'Windows', 'macOS'];
  const values = ['linux', 'windows', 'macOS'];
  return (
    <Tabs groupId="os" values={values.map((v, i) => ({value: v, label: labels[i]}))}>
      {items.map((child, i) => (
        <TabItem key={values[i]} value={values[i]}>{child}</TabItem>
      ))}
    </Tabs>
  );
}
